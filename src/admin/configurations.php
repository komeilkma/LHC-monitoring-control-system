<?php 
include("includes/controller.php");
$pagename = 'configurations';
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
                    <h2>General Settings</h2>
                    <ol class="breadcrumb">
                        <li>
                            <a href="index.php">Home</a>
                        </li>
                        <li class="active">
                            General Settings
                        </li>
                    </ol>
                </div>
                <!-- END Title Header -->
                
                <div class="row">                                     
                    <div class="col-sm-12 col-md-12">
                        <div class="panel">
                            <div class="panel-body">
                                <h4><strong>General Settings</strong> - Edit General Site Settings </h4>
                            </div>
                        </div>
                    </div>                                     
                </div>
             
                <div class="row">                    
                    <div class="col-sm-12 col-md-7 col-lg-8">
                        <div class="panel">
                            <div class="panel-heading">
                                <h2 class="panel-title">Configuration Settings</h2>
                            </div>
                            <div class="panel-body">
                                    <form class="form-horizontal" id="configurations-form-validation" role="form" action="includes/adminprocess.php" method="POST">                                                                      
                                        <div class="form-group <?php if(Form::error("sitename")) { echo 'has-error'; } ?>">
                                            <label for="sitename" class="col-sm-3 control-label">Site Name <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="sitename" id="sitename" placeholder="Required Field.." value="<?php if(Form::value('sitename') == ""){ echo $configs->getConfig('SITE_NAME'); } else { echo Form::value('sitename'); } ?>">
                                                    <span class="input-group-addon"><i class="oi oi-question-mark"></i></span>
                                                </div>
                                                <?php if(Form::error("sitename")) { echo "<div class='help-block' id='sitename-error'>".Form::error('sitename')."</div>"; } ?>
                                            </div>
                                        </div>  
                                        <div class="form-group <?php if(Form::error("sitedesc")) { echo 'has-error'; } ?>">
                                            <label for="sitedesc" class="col-sm-3 control-label">Site Description <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="sitedesc" id="sitedesc" placeholder="Required Field.." value="<?php if(Form::value("sitedesc") == ""){ echo $configs->getConfig('SITE_DESC'); } else { echo Form::value("sitedesc"); } ?>">
                                                    <span class="input-group-addon"><i class="oi oi-question-mark"></i></span>
                                                </div>
                                                <?php if(Form::error("sitedesc")) { echo "<div class='help-block' id='sitedesc-error'>".Form::error('sitedesc')."</div>"; } ?>
                                            </div>
                                        </div>
                                        <div class="form-group <?php if(Form::error("emailfromname")) { echo 'has-error'; } ?>">
                                            <label for="emailfromname" class="col-sm-3 control-label">E-mail From Name <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="emailfromname" id="emailfromname" placeholder="Required Field.." value="<?php if(Form::value("emailfromname") == ""){ echo $configs->getConfig('EMAIL_FROM_NAME'); } else { echo Form::value("emailfromname"); } ?>">
                                                    <span class="input-group-addon"><i class="oi oi-question-mark"></i></span>
                                                </div>
                                                <?php if(Form::error("emailfromname")) { echo "<div class='help-block' id='emailfromname-error'>".Form::error('emailfromname')."</div>"; } ?>
                                            </div>
                                        </div>
                                        <div class="form-group <?php if(Form::error("adminemail")) { echo 'has-error'; } ?>">
                                            <label for="adminemail" class="col-sm-3 control-label">Site E-mail Address <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="adminemail" id="adminemail" placeholder="Required Field.." value="<?php if(Form::value("adminemail") == ""){ echo $configs->getConfig('EMAIL_FROM_ADDR'); } else { echo Form::value("adminemail"); } ?>">
                                                    <span class="input-group-addon"><i class="oi oi-envelope-closed"></i></span>
                                                </div>
                                                <?php if(Form::error("adminemail")) { echo "<div class='help-block' id='adminemail-error'>".Form::error('adminemail')."</div>"; } ?>
                                            </div>
                                        </div> 
                                        <div class="form-group <?php if(Form::error("webroot")) { echo 'has-error'; } ?>">
                                            <label for="webroot" class="col-sm-3 control-label">Site Root <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="webroot" id="webroot" placeholder="Required Field.." value="<?php if(Form::value("webroot") == ""){ echo $configs->getConfig('WEB_ROOT'); } else { echo Form::value("webroot"); } ?>">
                                                    <span class="input-group-addon"><i class="oi oi-globe"></i></span>
                                                </div>
                                                <?php if(Form::error("webroot")) { echo "<div class='help-block' id='webroot-error'>".Form::error('webroot')."</div>"; } ?>
                                            </div>
                                        </div>
                                        <div class="form-group <?php if(Form::error("home_page")) { echo 'has-error'; } ?>">
                                            <label for="home_page" class="col-sm-3 control-label">Admin Home Page <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="home_page" id="home_page" placeholder="Required Field.." value="<?php echo $configs->getConfig('home_page'); ?>">
                                                    <span class="input-group-addon"><i class="oi oi-globe"></i></span>
                                                </div>
                                                <?php if(Form::error("home_page")) { echo "<div class='help-block' id='home_page-error'>".Form::error('home_page')."</div>"; } ?>
                                            </div>
                                        </div>
                                        <div class="form-group <?php if(Form::error("login_page")) { echo 'has-error'; } ?>">
                                            <label for="login_page" class="col-sm-3 control-label">Login Page <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="login_page" id="login_page" placeholder="Required Field.." value="<?php echo $configs->getConfig('login_page'); ?>">
                                                    <span class="input-group-addon"><i class="oi oi-globe"></i></span>
                                                </div>
                                            </div>
                                        </div> 
                                        <div class="form-group <?php if(Form::error("date_format")) { echo 'has-error'; } ?>">
                                            <label for="date_format" class="col-sm-3 control-label">PHP Date Format <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                    <input class="form-control" name="date_format" id="date_format" placeholder="Required Field.." value="<?php echo $configs->getConfig('date_format'); ?>">
                                                    <span class="input-group-addon"><i class="oi oi-calendar"></i></span>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="form-group <?php if(Form::error("hash")) { echo 'has-error'; } ?>">
                                            <label for="hash" class="col-sm-3 control-label">Hash <span class="text-danger">*</span></label>
                                            <div class="col-sm-8">
                                                <div class="input-group">
                                                <select name="hash" id="hash" class="form-control">                                         
                                                    <option value="sha256" <?php if ($configs->getConfig('HASH') == 'sha256') { echo "selected='selected'"; }?>>SHA256</option>
                                                    <option value="sha1" <?php if ($configs->getConfig('HASH') == 'sha1') { echo "selected='selected'"; }?>>SHA1</option>
                                                </select>
                                                    <span class="input-group-addon"><i class="oi oi-puzzle-piece"></i></span>
                                                </div>     
                                            </div>
                                        </div> 
                                        <div class="form-group">
                                            <div class="col-sm-offset-3 col-sm-10">
                                                <?php echo $adminfunctions->stopField($session->username, 'configs'); ?>
                                                <input type="hidden" name="form_submission" value="config_edit">
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
                                <strong>Site Name</strong> - Used in the title tags in the example pages and also in the mails that go out when users register and are activated.<br><br>
                                <strong>Site Description</strong> - Can be used to update site description tags.<br><br> 
                                <strong>E-mail From Name</strong> - Used as the e-mail Display Name in outgoing e-mails such as the welcome e-mail.<br><br>
                                <strong>Site E-mail Address</strong> - The Reply Address used in outgoing e-mails such as the welcome e-mail.<br><br>
                                <strong>Site Root</strong> - The absolute path to the admin directory. It must end with a trailing forward slash eg, http://www.website.com/admin/ <br><br>
                                <strong>Home Page</strong> - This page is appended to the Site Root (above) to make a full path of where your user is taken to after successfully logging on. <br><br> 
                                <strong>Login Page</strong> - This page is appended to the Site Root (above) to make a full path of where your user is taken to after successfully logging off.<br><br>
                                <strong>PHP Date Format</strong> - The format of the date shown throughout the admin pages (and potentially on your website) such as the Last Login Date. More details of how to set this field <a href="http://php.net/manual/en/function.date.php">here</a>.<br><br>
                                <strong>Hash</strong> - The hashing algorithm used to hash user's passwords. The setting is here for backward compatibility.
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
        <script src="js/plugins/formValidation/configurationsFormsValidation.js"></script>
        <script src="js/plugins/formValidation/jquery.validate.js"></script>
        <script>$(function() { FormsValidation.init(); });</script>        

    </body>
</html>
<?php
}
?>