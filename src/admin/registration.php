<?php 
include("includes/controller.php");
$pagename = 'registration';
$container = '';
if(!$session->isSuperAdmin()){
    header("Location: ".$configs->homePage());
    exit;
}
else{
$form = new Form();
?>
<!DOCTYPE html>
<html>
    <head>
       <title>IPM Software</title>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">

        <link href="css/bootstrap.min.css" rel="stylesheet">
        <link href="fonts/Open Iconic/css/open-iconic-bootstrap.min.css" rel="stylesheet">
        <link href="fonts/font-awesome/css/font-awesome.min.css" rel="stylesheet">

        <link href="css/navigation.css" rel="stylesheet">
        <link href="css/style.css" rel="stylesheet">
        <link href="css/animation.css" rel="stylesheet">
        
        <!-- Awesome Bootstrap Checkboxes CSS -->
        <link href="css/plugins/awesome-bootstrap-checkbox/awesome-bootstrap-checkbox.css" rel="stylesheet">     
        
    </head>
    <body>
        <!-- Page Wrapper -->
        <div id="page-wrapper">

            <!-- Side Menu -->
            <nav id="side-menu" class="navbar-default navbar-static-side" role="navigation">
                <div id="sidebar-collapse">
                    <div id="logo-element">
                        <a class="logo" href="index.php">
                            <img src="logo2.png">
                        </a>
                    </div>
                    <?php include('navigation.php'); ?>
                </div>
            </nav>
            <!-- END Side Menu -->

            <?php include('top-navbar.php'); ?>      

            <!-- Page Content -->
            <div id="page-content" class="gray-bg">

                <!-- Title Header -->
                <div class="title-header white-bg">
                    <i class="oi oi-star"></i>
                    <h2>Registration Settings</h2>
                    <ol class="breadcrumb">
                        <li>
                            <a href="index.php">Home</a>
                        </li>
                        <li class="active">
                            Registration Settings
                        </li>
                    </ol>
                </div>
                <!-- END Title Header -->
                
                <div class="row">                                     
                    <div class="col-sm-12 col-md-12">
                        <div class="panel">
                            <div class="panel-body">
                                <h4><strong>Registration Settings</strong> - Change the settings regarding registration to the site.</h4>
                            </div>
                        </div>
                    </div>                                     
                </div>
             
                <div class="row">                    
                    <div class="col-sm-12 col-md-7 col-lg-8">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">Registration Settings</h2>
                            </div>
                            <div class="panel-body">
                                    <form class="form-horizontal" id="registration-form-validation" role="form" action="includes/adminprocess.php" method="POST">                                                                      
                                            <div class="form-group">
                                                <label class="col-sm-4 control-label">Account Activation </label>
                                                <div class="col-sm-8">
                                                    <div class="radio radio-danger">
                                                        <input name="activation" id="activation4" type="radio" value="4" <?php if($configs->getConfig('ACCOUNT_ACTIVATION') == 4) { echo "checked='checked'"; } ?>>
                                                        <label for="activation4">
                                                            Disable Registration
                                                        </label>
                                                    </div>
                                                    <div class="radio radio-success">
                                                        <input name="activation" id="activation1" type="radio" value="1" <?php if($configs->getConfig('ACCOUNT_ACTIVATION') == 1) { echo "checked='checked'"; } ?>>
                                                        <label for="activation1">
                                                            No Activation (immediate access)
                                                        </label>
                                                    </div>
                                                    <div class="radio radio-warning">
                                                        <input name="activation" id="activation2" type="radio" value="2" <?php if($configs->getConfig('ACCOUNT_ACTIVATION') == 2) { echo "checked='checked'"; } ?>>
                                                        <label for="activation2">
                                                            User Activation (e-mail verification)
                                                        </label>
                                                    </div>
                                                    <div class="radio radio-warning">
                                                        <input name="activation" id="activation3" type="radio" value="3" <?php if($configs->getConfig('ACCOUNT_ACTIVATION') == 3) { echo "checked='checked'"; } ?>>
                                                        <label for="activation3">
                                                            Admin Activation
                                                        </label>
                                                    </div>
                                                </div>
                                            </div>
                                    <div class="form-group">
                                        <label for="limit_username_chars" class="col-sm-4 control-label">Limit Username Characters </label>
                                        <div class="col-sm-5">
                                            <select name="limit_username_chars" id="limit_username_chars" class="form-control">                                         
                                                <option value="any_chars" <?php if ($configs->getConfig('USERNAME_REGEX') == 'any_chars') { echo "selected='selected'"; }?>>Any Chars</option>
                                                <option value="alphanumeric_only" <?php if ($configs->getConfig('USERNAME_REGEX') == 'alphanumeric_only') { echo "selected='selected'"; }?>>Alphanumeric Only</option>
                                                <option value="alphanumeric_spacers" <?php if ($configs->getConfig('USERNAME_REGEX') == 'alphanumeric_spacers') { echo "selected='selected'"; }?>>Alphanumeric Spacers</option>
                                                <option value="any_letter_num" <?php if ($configs->getConfig('USERNAME_REGEX') == 'any_letter_num') { echo "selected='selected'"; }?>>Any Letter Num</option>
                                                <option value="letter_num_spaces" <?php if ($configs->getConfig('USERNAME_REGEX') == 'letter_num_spaces') { echo "selected='selected'"; }?>>Letter Num and Spaces</option>
                                            </select>
                                        </div>
                                    </div>
                                        <div class="form-group <?php if(Form::error("min_user_chars")) { echo 'has-error'; } else if (Form::error("max_user_chars")) { echo 'has-error'; } ?>">
                                            <label for="val_sitedescription" class="col-sm-4 control-label">Username Length <span class="text-danger">*</span></label>
                                            <div class="col-sm-5">
                                                <div class="input-group">
                                                    <input type="text" class="input-sm form-control" name="min_user_chars" id="min_user_chars" value="<?php if(Form::value("min_user_chars") == ""){ echo $configs->getConfig('min_user_chars'); } else { echo Form::value("min_user_chars"); } ?>"/>
                                                    <span class="input-group-addon">to</span>
                                                    <input type="text" class="input-sm form-control" name="max_user_chars" id="max_user_chars" value="<?php if(Form::value("max_user_chars") == ""){ echo $configs->getConfig('max_user_chars'); } else { echo Form::value("max_user_chars"); } ?>" />
                                                </div>
                                                <?php if(Form::error("min_user_chars")) { echo "<div class='help-block' id='min_user_chars'>".Form::error('min_user_chars')."</div>"; } else if(Form::error("max_user_chars")) { echo "<div class='help-block' id='max_user_chars'>".Form::error('max_user_chars')."</div>"; } ?>
                                            </div>
                                        </div>
                                        <div class="form-group <?php if(Form::error("min_pass_chars")) { echo 'has-error'; } else if (Form::error("max_pass_chars")) { echo 'has-error'; } ?>">
                                            <label for="val_sitedescription" class="col-sm-4 control-label">Password Length <span class="text-danger">*</span></label>
                                            <div class="col-sm-5">
                                                <div class="input-group">
                                                    <input type="text" class="input-sm form-control" name="min_pass_chars" id="min_pass_chars" value="<?php if(Form::value("min_pass_chars") == ""){ echo $configs->getConfig('min_pass_chars'); } else { echo Form::value("min_pass_chars"); } ?>"/>
                                                    <span class="input-group-addon">to</span>
                                                    <input type="text" class="input-sm form-control" name="max_pass_chars" id="max_pass_chars" value="<?php if(Form::value("max_pass_chars") == ""){ echo $configs->getConfig('max_pass_chars'); } else { echo Form::value("max_pass_chars"); } ?>" />
                                                </div>
                                                <?php if(Form::error("min_pass_chars")) { echo "<div class='help-block' id='min_pass_chars'>".Form::error('min_pass_chars')."</div>"; } else if(Form::error("max_pass_chars")) { echo "<div class='help-block' id='max_pass_chars'>".Form::error('max_pass_chars')."</div>"; } ?>
                                            </div>
                                        </div>
                                            <div class="form-group">
                                                <label class="col-sm-4 control-label">Send Welcome E-mail </label>
                                                <div class="col-sm-5">
                                                    <div class="radio radio-success radio-inline">
                                                        <input name="send_welcome" id="send_welcome1" type="radio" value="1" <?php if($configs->getConfig('EMAIL_WELCOME') == 1) { echo "checked='checked'"; } ?>>
                                                        <label for="send_welcome1">
                                                            Yes
                                                        </label>
                                                    </div>
                                                    <div class="radio radio-danger radio-inline">
                                                        <input name="send_welcome" id="send_welcome2" type="radio" value="0" <?php if($configs->getConfig('EMAIL_WELCOME') == 0) { echo "checked='checked'"; } ?>>
                                                        <label for="send_welcome2">
                                                            No
                                                        </label>
                                                    </div>
                                                </div>
                                            </div>
                                            <div class="form-group">
                                                <label class="col-sm-4 control-label">Enable Captcha </label>
                                                <div class="col-sm-5">
                                                    <div class="radio radio-success radio-inline">
                                                        <input name="enable_capthca" id="enable_capthca1" type="radio" value="1" <?php if($configs->getConfig('ENABLE_CAPTCHA') == 1) { echo "checked='checked'"; } ?>>
                                                        <label for="enable_capthca1">
                                                            Yes
                                                        </label>
                                                    </div>
                                                    <div class="radio radio-danger radio-inline">
                                                        <input name="enable_capthca" id="enable_capthca2"type="radio" value="0" <?php if($configs->getConfig('ENABLE_CAPTCHA') == 0) { echo "checked='checked'"; } ?>>
                                                        <label for="enable_capthca2">
                                                            No
                                                        </label>
                                                    </div>
                                                </div>
                                            </div>
                                            <div class="form-group">
                                                <label class="col-sm-4 control-label">Username Lowercase </label>
                                                <div class="col-sm-5">
                                                    <div class="radio radio-success radio-inline">
                                                        <input name="all_lowercase" value="1" <?php if($configs->getConfig('ALL_LOWERCASE') == 1) { echo "checked='checked'"; } ?> id="all_lowercase1" type="radio">
                                                        <label for="all_lowercase1">
                                                            Yes
                                                        </label>
                                                    </div>
                                                    <div class="radio radio-danger radio-inline">
                                                        <input name="all_lowercase" value="0" <?php if($configs->getConfig('ALL_LOWERCASE') == 0) { echo "checked='checked'"; } ?> id="all_lowercase2" type="radio">
                                                        <label for="all_lowercase2">
                                                            No
                                                        </label>
                                                    </div>
                                                </div>
                                            </div>
                                        <div class="form-group">
                                            <div class="col-sm-offset-4 col-sm-8">
                                                <?php echo $adminfunctions->stopField($session->username, 'registration'); ?>
                                                <input type="hidden" name="form_submission" value="registration_edit">
                                                <button type="submit" class="btn btn-default">Submit</button>
                                            </div>
                                        </div>
                                    </form>
                                </div>
                        </div>                    
                    </div>
                    <div class="col-sm-12 col-md-5 col-lg-4">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">Need Help ?</h2>
                            </div>
                            <div class="panel-body">
                                <strong>Account Activation</strong> - User Activation requires the new user to activate their account by clicking a link sent to their e-mail address. Admin Activation requires an admin to activate the account using the control panel or by a link sent to their e-mail address.<br><br>
                                <strong>Limit Username Characters</strong> - Limit the characters allowed in new username registrations.<br><br> 
                                <strong>Username Length</strong> - Minimum and maximum username length.<br><br>
                                <strong>Password Length</strong> - Minimum and maximum password length.<br><br>
                                <strong>Send Welcome E-mail</strong> - Whether or not to send a welcome e-mail to all new users upon registration.<br><br>
                                <strong>Enable Captcha</strong> - Do I want this?. <br><br> 
                                <strong>Username Lowercase</strong> - When set to yes, all registered usernames are made lowercase.
                            </div>
                        </div>
                    </div>
                </div>
                <!-- END Row -->

                <footer>Copyright &copy; <?php echo date("Y"); ?> <a href="http://ipm.ir" target="_blank">IPM</a> - All rights reserved. </footer>

            </div>
            <!-- END Page Content -->

            <?php include('rightsidebar.php'); ?>

        </div>
        <!-- END Page Wrapper -->
        
        <!-- Scroll to top -->
        <a href="#" id="to-top" class="to-top"><i class="oi oi-chevron-top"></i></a>

        <!-- JavaScript Resources -->
        <script src="js/jquery-2.1.3.min.js"></script>
        <script src="js/bootstrap.min.js"></script>
        <script src="js/plugins/metisMenu/jquery.metisMenu.js"></script>
        <script src="js/komeil.js"></script>
        
        <!-- Initialize Form Validation -->
        <script src="js/plugins/formValidation/registrationFormsValidation.js"></script>
        <script src="js/plugins/formValidation/jquery.validate.js"></script>
        <script>$(function() { FormsValidation.init(); });</script>

    </body>
</html>
<?php
}
?>